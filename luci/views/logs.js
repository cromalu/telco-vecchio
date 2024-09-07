'use strict';
'require view';
'require fs';
'require ui';

return view.extend({
    load: function() {
        return fs.exec_direct('tail',['-n','100','/tmp/log/telco-vecchio/log.1','/tmp/log/telco-vecchio/log']).catch(function(err) {
            ui.addNotification(null, E('p', {}, _('Unable to load logs: ' + err.message)));
            return '';
        });
    },

    render: function(logdata) {
        var loglines = logdata.split(/\n/).map(function(line) {
            return line.replace(/^<\d+>/, '');
        });

        var scrollDownButton = E('button', {
                'id': 'scrollDownButton',
                'class': 'cbi-button cbi-button-neutral',
            }, _('Scroll to tail', 'scroll to bottom (the tail) of the log file')
        );
        scrollDownButton.addEventListener('click', function() {
            scrollUpButton.scrollIntoView();
        });

        var scrollUpButton = E('button', {
                'id' : 'scrollUpButton',
                'class': 'cbi-button cbi-button-neutral',
            }, _('Scroll to head', 'scroll to top (the head) of the log file')
        );
        scrollUpButton.addEventListener('click', function() {
            scrollDownButton.scrollIntoView();
        });

        return E([], [
            E('h2', {}, [ _('Logs') ]),
            E('div', { 'id': 'content_syslog' }, [
                E('div', {'style': 'padding-bottom: 20px'}, [scrollDownButton]),
                E('textarea', {
                    'id': 'syslog',
                    'style': 'font-size:12px',
                    'readonly': 'readonly',
                    'wrap': 'off',
                    'rows': loglines.length + 1
                }, [ loglines.join('\n') ]),
                E('div', {'style': 'padding-bottom: 20px'}, [scrollUpButton])
            ])
        ]);
    },

    handleSaveApply: null,
    handleSave: null,
    handleReset: null
});